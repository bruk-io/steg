# The design session's working files

This directory is not Rust and is not consumed by any crate build. It is the durable
home for the working files behind the design artifact's three foundations pages
(Tokens, Layout, Details) only — not the whole artifact, which has more pages
(Atoms, Molecules, Organisms, States, Status, Storyboards, App, IconSet) that are out
of scope here. TASK-1.92 (doc-17, R121, R122) brought these in; a later task under
doc-17 can bring in the working files for another page the same way, if one is still
needed by then.

**The artifact (https://claude.ai/artifact/Ex9HHnDNY8C8fxV6BR4WVZ) is the source of
every named value, not anything in this directory** (decision-10). These files are
history: how the pages were drawn, and what they said, at two points in that history.
`crates/ui/design/foundations.json` is the structured export this task produced from
them; it is not generated from these files automatically, and these files are never
edited by hand to keep it in sync.

## What is here

- `foundations.json` — the original, sentence-based working data the design session
  drew the three foundations pages from (not the same file as, and not the same shape
  as, `crates/ui/design/foundations.json`). Copied unchanged from the backup kept
  outside the design session's temporary directory on 2026-11-15. **This file predates
  a correction later made to the artifact** (see "What changed since", below): its
  `scrim` sentence still says "Black at 60% in dark," and its drawing conventions still
  colour a disabled icon button with the `off` token. Read it as a historical record
  of what the pages first said, not as agreeing with `project/` below on every point.
- `gen_system.py` and `rebuild_main.py` (its one dependency) — the generator that drew
  the three foundations pages, copied unchanged from the same backup. **They are how
  the pages were first drawn, not a way to regenerate them today without losing a hand
  edit made since**: Bruk resized a few boxes on the Tokens page by hand in the design
  canvas after this generator last ran, so re-running it now would reproduce the
  pre-resize layout and lose that edit. Keep reading these scripts to understand how a
  value became a drawn page; don't run them expecting today's output.
- `project/Tokens.dc.html`, `project/Layout.dc.html`, `project/Details.dc.html` — the
  three foundations pages as saved at artifact version `1794791991-5d11`, after the
  correction below. This is the reference copy `crates/ui/design/foundations.json` was
  exported from and is checked against, and the reference copy for TASK-1.96's tool.

## What changed since the working `foundations.json` and the scripts above

Two things were corrected in the artifact after the files above were drawn, decided by
the engineer while this task was planned (TASK-1.92's implementation notes carry the
full reasoning):

1. **A disabled icon button's colour.** The generator above coloured an icon button's
   "cannot be used" state with a token named `off`, while every other disabled control
   already used `muted`, matching the design's own Surfaces rule ("a part that cannot
   be used takes the muted text colour"). The rule won: the artifact's `.lv-ib:disabled`
   rule is now `muted` on every page that shares it. `off` is still defined as a colour
   token in each page's own inline theme data — it is just never drawn by anything any
   more — so it is not in `crates/ui/design/foundations.json` either.
2. **The overlay's scrim base colour.** The working `foundations.json`'s `scrim`
   sentence said "Black at 60% in dark," but the pages always drew the overlay's scrim
   as `#050508` at 60%, not pure black. The drawing was right: the sentence on the
   Tokens and Layout pages now says `#050508 at 60% in dark` instead.

`project/` above already reflects both corrections; the working `foundations.json` and
the generator scripts do not, since they were not re-run. `crates/ui/design/
foundations.json` matches `project/`, not the working copy here, on these two points.

## Left out, and why

Narrower than the full backup this was copied from:

- `gen_icons.py`, `gen_app.py`, `gen_trial.py`, `apply_main.py`, `apply_refs.py` — these
  draw or patch other artifact pages (icons, the App/interactive mock, the texture
  trial board), not the three foundations pages. `apply_refs.py` in particular encodes
  a "status ball" design that predates the current ring-based status design (its
  patched text talks about balls; the current pages talk about rings) — committing it
  here under a "durable home" would misrepresent stale work as current.
- `*.before-*.bak` files — backups of earlier edits to the files above.
- `fairfax-hd-legacy.woff2` — a font found alongside the working files. Not committed:
  it is not a named value of the foundations, and its licence has not been checked.
- `test_app.js`, `app_logic.js` — belong to the App page's interactive mock, not the
  foundations pages.
