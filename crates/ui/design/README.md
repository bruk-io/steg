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
