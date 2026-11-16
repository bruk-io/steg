"""Check `crates/ui/design/foundations.json` against the saved foundations pages' rendered markup.

TASK-1.96 (doc-17, R121, R122). A named value in the data file is compared against the same
name's row on `design/canvas/project/Tokens.dc.html` or `Details.dc.html`, by three tiers:

- Tier A (`colour_mismatches`, `ansi_mismatches`): name and value match exactly, both
  directions. All 18 colours with a Name/Dark/Light/Used for row on Tokens.dc.html
  (`track` gained its own row at artifact version `1794796789-1b13`, closing what was
  a declared exception here - see the task's build notes), and all 32 ANSI colours
  (16 names x 2 themes) on Details.dc.html.
- Tier B (`sentence_mismatches`): the name is present and every number reachable in the
  file's entry is found somewhere in that same name's sentence cell - by default this
  checks the numbers only as a set, which is field-blind (a review finding: two of an
  entry's own fields swapped leaves the same numbers present and still passes).
  `FIELD_WORD_MAPS` tightens specific fields where the page's sentence ties a number to a
  distinguishing word right after it (`_word_tied_number`, `_tightened_mismatches`); a
  field not in that map, or whose word comes before its number or shares a clause with
  another number, stays on the weaker set check - printed as a caveat in `main()`'s
  passing output and in `crates/ui/design/README.md`, never silently assumed tightened.
  `TIER_B_NAMES` is the exhaustive list of file keys Tier B covers, one file key to one
  page row.
- Tier B, mapped (`mapped_sentence_mismatches`, `RADIUS_MAP`, `RADIUS_WORD_MAP`): the same
  check and the same tightening, but for a file key whose page row is named differently -
  `sizes.radius.surface/track/inner` (14/10/8) all check against Tokens.dc.html's one
  merged `radius` row. The engineer asked for this small, reviewed map rather than leaving
  all three as a declared exception; `radius.round` has no number on the page at all and
  stays in `TIER_C_SKIPS`.
- Tier C (`TIER_C_SKIPS`): every other named value in the file is declared out of this
  tool's reach, each with its own reason - printed by `main()` itself on every passing run,
  so a clean exit is never read as "every value checked" (AC#3). `unaccounted_names`
  walks the data file's own structure generically (`_leaf_paths` - no section's field
  names are written into this module as a list, a review finding: the first version
  enumerated `themes[]`/`focus_ring` from a fixed tuple, invisible to a field not already
  in it) and refuses if a leaf has no covering name in `checked` or `TIER_C_SKIPS` - a key
  added to the file later, at any level, cannot fall through both silently.

  Every path, in both the walk and the registry, is a tuple of segments - never a joined
  string (a further review finding: the data file's own keys contain literal dots -
  `font.ui`, `nav.side`, `radius.surface`, `ramp.extended` - so a string-prefix comparison
  cannot tell "one key with a dot in it" from "two nested keys" apart; it let
  `sizes["palette.footer"]`, `sizes["nav.side.extra"]`, `type["terminal.bold"]`, and a ramp
  literally named `"ramp.secondary"` all pass as covered by an unrelated registered name).
  `_is_accounted_for` compares whole segments only (`Skipped.display()` joins with "."
  purely for people to read, never for this comparison). Every colour, colour alias,
  opacity and ramp is registered field by field, never as a whole-entry ancestor (a third
  review finding: a blanket `("colours", name)` silently covered a stray field, such as
  `contrast`, that nothing actually checks - only `dark`/`light` are).

Only the rendered body is read - everything between `</helmet>` and the page's own
`<script type="text/x-dc">` block - never that script block itself: it holds the page's
own preview colour variables (`ink`, `off`, `quiet`, `faint`, ...), the page's vocabulary,
not the design's names, and parsing it would make values such as `off` (already decided
against in TASK-1.92) look like page-named values missing from the file.

The reverse direction (`reverse_missing`, `ansi_reverse_missing`) lists every name drawn
under a Name/Dark/Light/Used for, Name/Value, or ANSI swatch row and reports one with no
matching file entry - restricted to exactly those rows (never Layout.dc.html, never a
Rules bullet or a Stories caption). `main()` calls `reverse_missing` with `allow` set to
`REVERSE_ALLOW_LIST` (page names deliberately absent from the file - empty today) unioned
with `RADIUS_MAP.values()` (page names a map explains instead - `"radius"`): a name the
map matches is matched, not merely excused, so the two stay separate even though both end
up widening the same `allow` set. This is `sizes.radius.round`'s exact scenario from
TASK-1.92's review (a value drawn with no file entry): the three sized corners are now
mapped and checked; the fully round one stays a declared exception.

Agreement between the file and a page proves nothing about whether a value is part of the
*confirmed* design (TASK-1.92's one real miss, `overlay.layout.max_height_percent_of_window`,
was a value the page and the then-draft file agreed on; doc-10 flagged it as not yet
confirmed - a judgement this tool cannot make). This is a consistency check between two
documents, not a substitute for reading doc-10's open-questions list.

The plan named TASK-1.94's `steg_tools.foundations.read()` as a shared reader this task
would import once merged. Now that it has (its `Foundations` dataclass holds `colours`
and `opacity_names` only - R120's colour-literal check, AC#2, is TASK-1.94's whole scope),
it covers a narrow slice of what this module reads: colours, but not `type`, `sizes`,
`themes`, `motion`, `ramps`, `colour_aliases`, `tint` or `focus_ring`, every one of which
Tier B, `mapped_sentence_mismatches` and `unaccounted_names` need as raw JSON. Importing
it for colours alone would mean reading the file twice down two different paths for no
real gain, so `_read_data_file` below stays the one read of the whole document - not an
oversight, a decision made once TASK-1.94's actual shape was known.

Usage: steg-design-check [--data-file <path>]
"""

import json
import re
import sys
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path

from steg_tools import workspace

# ---------------------------------------------------------------------------
# Parsed shapes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Node:
    """One element in a saved page's rendered body: its tag and its children, text or nested elements."""

    tag: str
    children: tuple["Node | str", ...]


@dataclass(frozen=True)
class ColourRow:
    """One named colour's dark and light hex, as drawn in a Name/Dark/Light/Used for table."""

    name: str
    dark: str
    light: str


@dataclass(frozen=True)
class SentenceRow:
    """One named value's free-text cell, as drawn in a Name/Value table."""

    name: str
    text: str


@dataclass(frozen=True)
class AnsiRow:
    """One terminal theme's one named ANSI colour, as drawn in a swatch card."""

    theme: str
    name: str
    hex: str


@dataclass(frozen=True)
class Page:
    """Everything this tool can read out of one saved page's rendered body."""

    colours: tuple[ColourRow, ...]
    sentences: tuple[SentenceRow, ...]
    ansi: tuple[AnsiRow, ...]


@dataclass(frozen=True)
class Mismatch:
    """A named value whose form on the page doesn't match the data file - or isn't on the page at all."""

    name: str
    file_form: str
    page_form: str
    page: str


@dataclass(frozen=True)
class Missing:
    """A value drawn under a name on a page with no entry in the data file."""

    name: str
    page_form: str
    page: str


@dataclass(frozen=True)
class Skipped:
    """A data-file value this tool declares out of its reach, and why (AC#3).

    `name` is a tuple of segments as the JSON has them - never a joined string (a review
    finding: `font.ui`, `nav.side`, `radius.surface`, `ramp.extended` are each one real
    JSON key that happens to contain a literal dot, so a joined-string comparison cannot
    tell "one key with a dot in it" from "two nested keys" apart). `display()` joins them
    with "." for people to read; nothing compares against that joined form.
    """

    name: tuple[str, ...]
    reason: str

    def display(self) -> str:
        """`name`, joined with "." for printing only - never for comparison."""
        return ".".join(self.name)


# ---------------------------------------------------------------------------
# Parsing: the rendered body only, via the standard library's parser
# ---------------------------------------------------------------------------

_BODY_START = "</helmet>"
_BODY_END = '<script type="text/x-dc"'


def rendered_body(html: str) -> str:
    """Slice out the rendered body between `</helmet>` and the page's interactive script block.

    Deliberately excludes the script block and everything before it (`<head>`,
    `<helmet>`'s own `<link>`/`<style>`) - see the module docstring for why the script
    block is never page content. Excluding `<helmet>` also means the tree builder below
    never meets a void element (`<link>`, `<meta>`): neither occurs in the body region.

    Raises:
        ValueError: either marker is missing - the page's shape changed enough that this
            tool can no longer find its own reading boundary.
    """
    start = html.find(_BODY_START)
    end = html.find(_BODY_END)
    if start == -1 or end == -1 or end <= start:
        raise ValueError(f"could not find the rendered body between {_BODY_START!r} and {_BODY_END!r}")
    return html[start + len(_BODY_START) : end]


class _MutableNode:
    """The tree builder's own working node - mutable while parsing, frozen into `Node` once done."""

    __slots__ = ("tag", "children")

    def __init__(self, tag: str) -> None:
        self.tag = tag
        self.children: list[_MutableNode | str] = []


def _freeze(node: _MutableNode | str) -> Node | str:
    if isinstance(node, str):
        return node
    return Node(node.tag, tuple(_freeze(child) for child in node.children))


class _TreeBuilder(HTMLParser):
    """Build a `Node` tree from a rendered-body HTML fragment (no void elements expected there)."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._root = _MutableNode("#root")
        self._stack: list[_MutableNode] = [self._root]

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        node = _MutableNode(tag)
        self._stack[-1].children.append(node)
        self._stack.append(node)

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self._stack[-1].children.append(_MutableNode(tag))

    def handle_endtag(self, tag: str) -> None:
        # Pop back to (and including) the matching open tag - tolerant of an unmatched
        # close so a malformed fragment (a unit test's typo, never the real pages) can't
        # desync the whole rest of the tree.
        for depth in range(len(self._stack) - 1, 0, -1):
            if self._stack[depth].tag == tag:
                del self._stack[depth:]
                return

    def handle_data(self, data: str) -> None:
        text = data.strip()
        if text:
            self._stack[-1].children.append(text)

    def root(self) -> Node:
        frozen = _freeze(self._root)
        assert isinstance(frozen, Node)
        return frozen


def build_tree(html: str) -> Node:
    """Parse one rendered-body HTML fragment into a `Node` tree."""
    builder = _TreeBuilder()
    builder.feed(html)
    builder.close()
    return builder.root()


def text_content(node: Node | str) -> str:
    """Every bit of text under `node`, concatenated in document order."""
    if isinstance(node, str):
        return node
    return "".join(text_content(child) for child in node.children)


def _direct_spans(node: Node) -> tuple[Node, ...]:
    return tuple(child for child in node.children if isinstance(child, Node) and child.tag == "span")


def cell_tuple(row: Node) -> tuple[str, ...]:
    """A table row's cell text, one entry per direct `<span>` child, in column order."""
    return tuple(text_content(span).strip() for span in _direct_spans(row))


def _iter_divs(node: Node) -> list[Node]:
    found: list[Node] = []
    for child in node.children:
        if isinstance(child, Node):
            if child.tag == "div":
                found.append(child)
            found.extend(_iter_divs(child))
    return found


HEADER_COLOUR = ("Name", "Dark", "Light", "Used for")
HEADER_SENTENCE = ("Name", "Value")


def find_table_rows(root: Node, header: tuple[str, ...]) -> tuple[tuple[str, ...], ...]:
    """Every data row of every table under `root` whose first row's cells are exactly `header`.

    Keyed on the header row's own cell text, never on `grid-template-columns` - a hand
    resize (`design/canvas/README.md`) changes the pixel string, never the column
    headings.
    """
    rows: list[tuple[str, ...]] = []
    for div in _iter_divs(root):
        row_divs = tuple(child for child in div.children if isinstance(child, Node) and child.tag == "div")
        if row_divs and cell_tuple(row_divs[0]) == header:
            rows.extend(cell_tuple(row) for row in row_divs[1:] if len(cell_tuple(row)) == len(header))
    return tuple(rows)


_HEX_RE = re.compile(r"^#[0-9a-fA-F]{6}$")


def find_ansi_cards(root: Node) -> tuple[tuple[str, str], ...]:
    """Every (name, hex) swatch card under `root`: a div of exactly 3 `<span>` children shaped
    (empty swatch, name text, `#rrggbb` text) - Details.dc.html's per-colour cards.
    """
    found: list[tuple[str, str]] = []
    for div in _iter_divs(root):
        spans = _direct_spans(div)
        if len(spans) != 3:
            continue
        swatch, name, hexcode = (text_content(span).strip() for span in spans)
        if swatch == "" and name and _HEX_RE.fullmatch(hexcode):
            found.append((name, hexcode))
    return tuple(found)


def find_sections(root: Node) -> tuple[tuple[str, Node], ...]:
    """Every `<section>` under `root`, paired with its first `<h2>`'s text."""
    found: list[tuple[str, Node]] = []
    for child in root.children:
        if not isinstance(child, Node):
            continue
        if child.tag == "section":
            found.append((_first_heading(child), child))
        else:
            found.extend(find_sections(child))
    return tuple(found)


def _first_heading(node: Node) -> str:
    for child in node.children:
        if isinstance(child, Node):
            if child.tag == "h2":
                return text_content(child).strip()
            heading = _first_heading(child)
            if heading:
                return heading
    return ""


_THEME_HEADING_RE = re.compile(r"^Terminal theme: (.+)$")


def parse_page(html: str) -> Page:
    """Parse one saved page's rendered body into every colour/sentence/ANSI row it draws.

    A page with none of these shapes parses to an empty `Page` - `shape_problems` is what
    turns "found nothing" into a refusal; this function alone never does.
    """
    root = build_tree(rendered_body(html))
    colours = tuple(ColourRow(row[0], row[1], row[2]) for row in find_table_rows(root, HEADER_COLOUR))
    sentences = tuple(SentenceRow(row[0], row[1]) for row in find_table_rows(root, HEADER_SENTENCE))
    ansi: list[AnsiRow] = []
    for heading, section in find_sections(root):
        match = _THEME_HEADING_RE.match(heading)
        if match is not None:
            theme = match.group(1)
            ansi.extend(AnsiRow(theme, name, hexcode) for name, hexcode in find_ansi_cards(section))
    return Page(colours, sentences, tuple(ansi))


# ---------------------------------------------------------------------------
# Shape: a table or card grid that comes back empty (or near-empty) is a refusal
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ExpectedShape:
    """One page's asserted minimum row count, so a parser that silently finds nothing refuses
    loudly instead of comparing an empty table and passing.
    """

    page: str
    kind: str
    minimum: int


EXPECTED_SHAPES = (
    # 18, not 17: the "Surfaces and text" table gained an 11th row (track) at artifact
    # version 1794796789-1b13 - this total also covers "the first colours table's
    # minimum becomes at least 11" (the engineer's wording), since a drop back to 10
    # there would bring the page-wide total below 18 even if "The six colours" table
    # (7 rows) stayed full.
    ExpectedShape("Tokens.dc.html", "colours", 18),
    ExpectedShape("Tokens.dc.html", "sentences", 17),
    ExpectedShape("Details.dc.html", "ansi", 32),
)


def shape_problems(pages: dict[str, Page]) -> tuple[str, ...]:
    """Say which page's row kind came back under its asserted minimum (or the page is absent)."""
    problems = []
    for expected in EXPECTED_SHAPES:
        page = pages.get(expected.page)
        count = 0 if page is None else len(getattr(page, expected.kind))
        if count < expected.minimum:
            problems.append(f"{expected.page}: found {count} {expected.kind} rows, expected at least {expected.minimum}")
    return tuple(problems)


# ---------------------------------------------------------------------------
# Tier C: declared, out of reach, with a reason - see the module docstring
# ---------------------------------------------------------------------------

TIER_C_SKIPS: tuple[Skipped, ...] = (
    # colours.track was here (no named row) until artifact version 1794796789-1b13,
    # when the engineer added a track row to Tokens.dc.html's first colours table -
    # track is now checked like every other colour (Tier A), no exception left.
    #
    # colour_aliases/opacities/ramps/motion are registered field by field, never as a
    # whole-entry ancestor (`("colour_aliases", "blocked")` alone) - a review finding: a
    # whole-entry ancestor silently swallows a stray new field nothing actually checks
    # (the same bug fixed for colours in `unaccounted_names`). Each alias's one real
    # field is its `ref`; each opacity's are `dark`/`light`; each ramp's are `stops` and,
    # on "ramp" only, `caption_opacity`; motion's one entry's is `duration`.
    Skipped(("colour_aliases", "blocked", "ref"), "a reference to ramp.5 amber, never drawn under its own name"),
    Skipped(("colour_aliases", "done", "ref"), "a reference to ramp.6 green, never drawn under its own name"),
    Skipped(("colour_aliases", "error", "ref"), "a reference to ramp.3 red, never drawn under its own name"),
    Skipped(("opacities", "faint", "dark"), "drawn only in each page's own script-block colour variables (0.3/0.45), never under its own name"),
    Skipped(("opacities", "faint", "light"), "drawn only in each page's own script-block colour variables (0.3/0.45), never under its own name"),
    Skipped(("ramps", "ramp", "stops"), "the gradient is drawn; its per-stop percentages are only in the page's own script block"),
    Skipped(("ramps", "ramp", "caption_opacity"), "50%/40% is in Rules prose with no name column"),
    Skipped(("ramps", "ramp.extended", "stops"), "the gradient is drawn; its per-stop percentages are only in the page's own script block"),
    Skipped(
        ("tint",),
        'drawn as Layout.dc.html\'s Track card row named "fill", not under the name "tint" - '
        "and Layout.dc.html is out of scope entirely (its own text: \"values... are on the "
        'first Foundations board"); see sizes.row.gap below',
    ),
    Skipped(("motion", "motion.status-fade", "duration"), "200ms is in Rules prose with no name column"),
    Skipped(("focus_ring", "width"), "2px is in the Focus ring section's prose (\"A 2px ring in the session colour\"), no name column"),
    Skipped(("focus_ring", "offset"), "2px clear of the control is in the Focus ring section's prose, no name column"),
    Skipped(("focus_ring", "colour"), "the literal sentinel \"session\" (R26): the active session's own colour, assigned at runtime, not a fixed page form"),
    Skipped(("focus_ring", "corner_radius_addition"), "derived (12px corner minus 8px corner); 4 never appears as a number on the page"),
    # Per theme instance, not a "themes[]" wildcard: a generic leaf walk (unaccounted_names)
    # has no way to collapse two theme instances into one pattern without re-introducing
    # the per-section field list this registry exists to avoid - so each of the two
    # themes' five prose-only fields has its own entry.
    Skipped(("themes", "steg dark", "bg"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg dark", "fg"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg dark", "bg_inactive"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg dark", "selection"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg dark", "cursor"), "the literal sentinel \"session\" (R26), the same as focus_ring.colour - no fixed page form"),
    Skipped(("themes", "steg light", "bg"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg light", "fg"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg light", "bg_inactive"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg light", "selection"), "a trailing prose line per theme, not a named row"),
    Skipped(("themes", "steg light", "cursor"), "the literal sentinel \"session\" (R26), the same as focus_ring.colour - no fixed page form"),
    # sizes.radius.surface/track/inner were here until the engineer asked for a small,
    # reviewed name map instead (RADIUS_MAP below): each now checks its own number
    # against Tokens.dc.html's one merged "radius" row (14/10/8).
    Skipped(("sizes", "radius.round"), 'drawn as an unlabelled swatch ("fully round: chips of colour, bars"), no px number and no name at all'),
    Skipped(("sizes", "row.gap"), "only on Layout.dc.html, out of scope entirely"),
    Skipped(("sizes", "stack.gap"), "only on Layout.dc.html, out of scope entirely"),
    Skipped(("sizes", "track.layout"), "only on Layout.dc.html, out of scope entirely"),
    Skipped(("sizes", "split.layout"), "only on Layout.dc.html, out of scope entirely"),
    Skipped(("sizes", "overlay.layout"), "only on Layout.dc.html, out of scope entirely"),
    Skipped(("session_colour",), "a top-level pointer to the literal sentinel \"session\" (R26), not a fixed page form - same sentinel as each theme's cursor and focus_ring.colour"),
)


def _skipped_colour_names() -> frozenset[str]:
    """Every colour name `TIER_C_SKIPS` declares unseen as a whole - a 2-segment
    `("colours", name)` entry, not the 3-segment `("colours", name, "dark"|"light")` form
    every checked colour's own fields are registered under. None today: `track` was the
    last one, closed at artifact version 1794796789-1b13.
    """
    return frozenset(skip.name[1] for skip in TIER_C_SKIPS if len(skip.name) == 2 and skip.name[0] == "colours")


SKIPPED_COLOURS = _skipped_colour_names()

# A page name deliberately absent from the file, exempted from the reverse direction.
# Empty today: "radius" was here until RADIUS_MAP (below) gave it a real match instead
# of an exemption - `reverse_missing`'s caller unions this with `RADIUS_MAP.values()`,
# so a page row the map explains is matched, not merely excused.
REVERSE_ALLOW_LIST: frozenset[str] = frozenset()

# A file key whose page row is named differently from the key itself - a small, reviewed
# map (the engineer's decision on TASK-1.96), not a general mechanism: Tokens.dc.html
# merges three corner sizes into one row named "radius" ("14px panes and palette; 10px
# tab track and session chips; 8px inside them"). sizes.radius.round has no number on the
# page at all (TIER_C_SKIPS) and is deliberately not in this map.
RADIUS_MAP = {
    "radius.surface": "radius",
    "radius.track": "radius",
    "radius.inner": "radius",
}

# The 2-column file keys this tool checks by name + every number in the file's entry
# (Tier B). Everything else under `type`/`sizes` is Tier C (see TIER_C_SKIPS) or simply
# not drawn on a page this tool reads.
TIER_B_NAMES = (
    "font.ui",
    "font.terminal",
    "terminal",
    "ui.body",
    "ui.small",
    "ui.label",
    "palette.input",
    "space",
    "nav.side",
    "nav.top",
    "tab.width",
    "pane.gap",
    "pane.caption",
    "palette",
    "status.ring",
    "scrim",
)


# Fields that describe the value beside them rather than being one, or that another
# mechanism already checks structurally - a small, universal, stated rule, applied by
# `_leaf_paths` to every dict at every level; no section's field names are written into
# this module as a list.
_NOT_A_LEAF_PATH = frozenset(
    {
        "name",  # identifies the entry whose own name the path already carries
        "use",  # prose describing the value beside it, never itself a value
        "unit",  # the measurement unit of a number beside it, never itself a value
        "ansi",  # each theme's 16 ANSI colours - checked structurally by
        # `ansi_mismatches` (Tier A), not by this name-based registry
        "artifact_version",  # this export's own provenance, not a design value
        "artifact_url",  # same
    }
)


def _leaf_paths(value: object, path: tuple[str, ...]) -> list[tuple[str, ...]]:
    """Every leaf path under `value`, walked generically from the data itself, as a tuple
    of segments - one JSON key or entry name per segment, never split on an internal dot,
    and never joined into a string for this function's own recursion:

    - a dict's keys become `(*path, key)` (skipping `_NOT_A_LEAF_PATH`, which also drops a
      list entry's own `name` once it has been used as the next segment below);
    - a non-empty list whose items are all dicts carrying their own `name` is a list of
      named entries: one path per entry, `(*path, item["name"])` - `name` is used whole,
      however many dots it contains (`motion`'s own entry is `"motion.status-fade"`, one
      segment, not re-split), recursing into that entry's whole dict the same way;
    - anything else reached this way - a scalar, a `{"ref": ...}` object, a plain list of
      numbers or strings, or a list of un-named dicts - is one leaf at `path`.
    """
    if isinstance(value, dict):
        found = []
        for key, sub in value.items():
            if key in _NOT_A_LEAF_PATH:
                continue
            found.extend(_leaf_paths(sub, (*path, key)))
        return found
    if isinstance(value, list) and all(isinstance(item, dict) and "name" in item for item in value):
        # `all()` on an empty list is vacuously true - an empty list of named entries
        # has zero of them, not one leaf for the empty container itself.
        found = []
        for item in value:
            found.extend(_leaf_paths(item, (*path, item["name"])))
        return found
    return [path]


def _is_accounted_for(path: tuple[str, ...], registered: frozenset[tuple[str, ...]]) -> bool:
    """`path` is accounted for if it equals, or has as a *whole-segment* prefix, some path
    in `registered`. Comparison is segment by segment, never on the joined string: `nav.
    side` is one segment, so `("sizes", "nav.side.extra")` is never mistaken for a child
    of `("sizes", "nav.side")` just because the joined strings happen to share a prefix (a
    review finding this exists to fix - string-prefix matching let `sizes["palette.
    footer"]`, `type["terminal.bold"]`, `sizes["nav.side.extra"]` and a ramp literally
    named `"ramp.secondary"` all pass as "covered by" an unrelated registered name).

    A whole-segment prefix is accounted for because the checking function that
    registered name stands for already recurses the whole value under it (`_numbers_in`
    sees every number under `("sizes", "nav.side")`, `ansi_mismatches` every ANSI colour)
    - only a path with no covering ancestor at all is new.
    """
    return any(path[: len(name)] == name for name in registered)


def unaccounted_names(
    data: dict,
    skipped: frozenset[tuple[str, ...]] = frozenset(skip.name for skip in TIER_C_SKIPS),
    checked: frozenset[str] = frozenset(TIER_B_NAMES) | frozenset(RADIUS_MAP),
) -> tuple[str, ...]:
    """Every leaf path in `data` with no covering name in `checked` or `skipped` - empty
    when every leaf key is accounted for, one way or the other (a key added to the real
    data file later and left off both shows up here, named exactly - joined with "." only
    for the string this returns, never for the comparison that found it).

    Discovery is generic: `_leaf_paths` walks `data` itself as tuples of segments, so a
    field this module has never heard of is found regardless of where it sits, and a
    name that merely shares a string prefix with a registered one is never mistaken for
    its descendant (both were review findings on this task).

    Accounting stays the explicit, reviewed registry this module already keeps
    (`TIER_B_NAMES`/`RADIUS_MAP`/`TIER_C_SKIPS`): `checked`'s bare `type`/`sizes` keys are
    translated to a `("type"|"sizes", key)` tuple by checking which of `data["type"]`/
    `data["sizes"]` actually holds that key - read from the data, not asserted. A leaf
    *inside* one of these whole values (e.g. a new number added inside `sizes.nav.side`)
    is accounted for by that covering name, not flagged here, since `_numbers_in` already
    recurses the whole value - nothing numeric there goes uninspected. A new *non-numeric*
    field added the same way is not inspected either, and this function does not catch
    that case: a known, accepted gap of the same shape as Tier B's own field-blindness,
    not a silent one (crates/ui/design/README.md).

    Every colour, colour alias, opacity, ramp and theme is registered field by field
    (`("colours", name, "dark")`, never the whole-entry `("colours", name)`) - a review
    finding: a blanket whole-entry ancestor silently swallowed a stray new field
    (`colours[].contrast`) that nothing actually checks. Only the fields a checking
    function reads, or that `TIER_C_SKIPS` names individually, are registered; anything
    else on such an entry is unaccounted, even though the entry's own name is known.
    """
    registered: set[tuple[str, ...]] = set(skipped)
    for name in checked:
        if name in data.get("type", {}):
            registered.add(("type", name))
        if name in data.get("sizes", {}):
            registered.add(("sizes", name))
    for colour in data.get("colours", []):
        if colour["name"] not in SKIPPED_COLOURS:
            registered.add(("colours", colour["name"], "dark"))
            registered.add(("colours", colour["name"], "light"))
    registered_frozen = frozenset(registered)

    leaves = _leaf_paths(data, ())
    return tuple(".".join(path) for path in leaves if not _is_accounted_for(path, registered_frozen))


# ---------------------------------------------------------------------------
# Tier A / Tier B / reverse direction - pure comparison, no I/O
# ---------------------------------------------------------------------------


def colour_mismatches(file_colours: tuple[dict, ...], page: Page, page_name: str) -> tuple[Mismatch, ...]:
    """Tier A, colours: every file colour (but `SKIPPED_COLOURS`) against its named page row."""
    by_name = {row.name: row for row in page.colours}
    found = []
    for entry in file_colours:
        name = entry["name"]
        if name in SKIPPED_COLOURS:
            continue
        file_form = f"dark={entry['dark']} light={entry['light']}"
        page_row = by_name.get(name)
        if page_row is None:
            found.append(Mismatch(name, file_form, "<no row found>", page_name))
            continue
        if page_row.dark.lower() != entry["dark"].lower() or page_row.light.lower() != entry["light"].lower():
            found.append(Mismatch(name, file_form, f"dark={page_row.dark} light={page_row.light}", page_name))
    return tuple(found)


def ansi_mismatches(file_themes: tuple[dict, ...], page: Page, page_name: str) -> tuple[Mismatch, ...]:
    """Tier A, ANSI colours: every theme's 16 named colours against that theme's page cards."""
    found = []
    for theme in file_themes:
        theme_name = theme["name"]
        by_name = {row.name: row.hex for row in page.ansi if row.theme == theme_name}
        for key, hexcode in theme["ansi"].items():
            name = key.replace("_", " ")
            page_hex = by_name.get(name)
            if page_hex is None:
                found.append(Mismatch(f"{theme_name}.{key}", hexcode, "<no row found>", page_name))
            elif page_hex.lower() != hexcode.lower():
                found.append(Mismatch(f"{theme_name}.{key}", hexcode, page_hex, page_name))
    return tuple(found)


def _numbers_in(value: object) -> frozenset[float]:
    """Every number reachable in a JSON value, scaling an `opacity` leaf to a whole-number
    percent - the design always writes opacity that way in prose (`scrim`'s row says "60%",
    never "0.6"); every other number is taken as written. A `{"ref": ...}` object
    contributes no number: it has no int/float leaf.
    """
    found: set[float] = set()

    def walk(value: object, key: str | None) -> None:
        if isinstance(value, bool):
            return
        if isinstance(value, int | float):
            # Rounded to the nearest whole percent: the design always writes an opacity
            # that way in prose, and leaving the raw `* 100` risks a binary-float
            # imprecision (0.28 * 100 == 28.000000000000004) that would never equal the
            # whole number the page's regex-extracted text gives back.
            found.add(round(float(value) * 100) if key == "opacity" else float(value))
        elif isinstance(value, dict):
            for sub_key, sub_value in value.items():
                walk(sub_value, sub_key)
        elif isinstance(value, list):
            for item in value:
                walk(item, key)

    walk(value, None)
    return frozenset(found)


_NUMBER_RE = re.compile(r"\d+\.\d+|\d+")


def _numbers_in_text(text: str) -> frozenset[float]:
    return frozenset(float(match) for match in _NUMBER_RE.findall(text))


_CLAUSE_SPLIT_RE = re.compile(r"[,;.]")


def _single_number_clauses(text: str) -> tuple[tuple[float, str], ...]:
    """Each `,`/`;`/`.`-separated clause of `text` with exactly one number in it, paired
    with the text that follows that number within the clause.

    A clause with zero or two-or-more numbers is left out entirely: attributing a word to
    one of several numbers in the same clause ("tabs 30px in a track with a 2px inset")
    would be a guess, not a reading. `FIELD_WORD_MAPS`/`RADIUS_WORD_MAP` rely on this: a
    field whose clause doesn't take this shape simply isn't tightened, and falls back to
    the weaker "present somewhere" check `sentence_mismatches`/`mapped_sentence_mismatches`
    already do.
    """
    pairs = []
    for clause in (c.strip() for c in _CLAUSE_SPLIT_RE.split(text) if c.strip()):
        numbers = _NUMBER_RE.findall(clause)
        if len(numbers) != 1:
            continue
        match = _NUMBER_RE.search(clause)
        assert match is not None
        remainder = clause[match.end() :].strip()
        if remainder:
            pairs.append((float(numbers[0]), remainder))
    return tuple(pairs)


def _word_tied_number(text: str, word: str) -> float | None:
    """The number in a single-number clause of `text` whose remainder contains `word`, or
    `None` if no such clause exists (the sentence gives no word to tie to - the caller
    falls back to the weak check rather than treat this as a mismatch).
    """
    for number, remainder in _single_number_clauses(text):
        if word in remainder:
            return number
    return None


# A small, reviewed map from one field of a `TIER_B_NAMES` entry's value to the word that
# follows its number in the page's sentence - tightens the otherwise field-blind "every
# number is present somewhere" check (review finding on TASK-1.96: swapping sizes.tab.
# width's usual/min left the same two numbers present and still passed). Built by hand,
# one row at a time, against the real pages - not a general NLP parse: a field is in here
# only where its clause unambiguously has one number followed by one identifying word.
# Every field *not* listed here (the majority: a number precedes its word, or shares a
# clause with another number) stays on the weaker "present somewhere" check alone, printed
# as a caveat in the tool's own output (see `main`) and in crates/ui/design/README.md.
FIELD_WORD_MAPS: dict[str, dict[str, str]] = {
    "tab.width": {"usual": "usual", "max": "at most", "min": "at least"},
    "nav.side": {"width": "wide"},
    "nav.top": {"height": "high"},
    "pane.gap": {"resting": "between"},
    "palette": {"width": "wide", "top_offset": "from the top"},
    "status.ring": {"diameter": "across", "thickness": "thick"},
}


def _tightened_mismatches(name: str, value: object, page_row: SentenceRow, page_name: str) -> tuple[Mismatch, ...]:
    """`FIELD_WORD_MAPS`'s per-field tightening for one `TIER_B_NAMES` entry already known
    to have a page row - a field whose word ties to a different number than the file gives
    it is a mismatch even though the plain "present somewhere" check above it never sees
    one (both numbers are still present, just swapped between fields).
    """
    word_map = FIELD_WORD_MAPS.get(name)
    if word_map is None or not isinstance(value, dict):
        return ()
    found = []
    for field, word in word_map.items():
        required = value.get(field)
        if not isinstance(required, int | float):
            continue
        tied = _word_tied_number(page_row.text, word)
        if tied is None:
            continue  # the sentence gives no clean single-number clause for this word
        if tied != float(required):
            found.append(Mismatch(f"{name}.{field}", f"{required} (tied to {word!r})", f"{word!r} is next to {tied}", page_name))
    return tuple(found)


def sentence_mismatches(file_type: dict, file_sizes: dict, page: Page, page_name: str) -> tuple[Mismatch, ...]:
    """Tier B: every `TIER_B_NAMES` entry present in the file against its named page row.

    A name with no numbers in the file (`font.ui`, `font.terminal`) degrades to a
    name-presence check only - documented here, not hidden. `FIELD_WORD_MAPS` tightens
    the entries it covers (see `_tightened_mismatches`); every other entry's numbers are
    only checked as a set, so two numbers swapped between two of its fields is invisible
    to this function alone - also documented, not hidden (`main`'s printed caveat).
    """
    by_name = {row.name: row for row in page.sentences}
    source = {**file_type, **file_sizes}
    found = []
    for name in TIER_B_NAMES:
        value = source.get(name)
        if value is None:
            continue  # absent from the file entirely - not this check's job to flag
        page_row = by_name.get(name)
        if page_row is None:
            found.append(Mismatch(name, repr(value), "<no row found>", page_name))
            continue
        needed = _numbers_in(value)
        if needed:
            have = _numbers_in_text(page_row.text)
            missing = needed - have
            if missing:
                found.append(Mismatch(name, f"numbers {sorted(needed)}", page_row.text, page_name))
        found.extend(_tightened_mismatches(name, value, page_row, page_name))
    return tuple(found)


# RADIUS_MAP's own small, reviewed word map (same idea as FIELD_WORD_MAPS, but keyed by
# the whole file key rather than a field of one entry's dict, since each of these three
# file keys is itself a single number, not a dict of several).
RADIUS_WORD_MAP = {
    "radius.surface": "palette",
    "radius.track": "tab track",
    "radius.inner": "inside",
}


def mapped_sentence_mismatches(file_sizes: dict, mapping: dict[str, str], page: Page, page_name: str) -> tuple[Mismatch, ...]:
    """Tier B, mapped: like `sentence_mismatches`, but the page row's name differs from the
    file key (`RADIUS_MAP`) - several file keys can map to the same page row, so
    `RADIUS_WORD_MAP` tightens the otherwise field-blind check the same way
    `FIELD_WORD_MAPS` does for `sentence_mismatches`.
    """
    by_name = {row.name: row for row in page.sentences}
    found = []
    for file_key, page_row_name in mapping.items():
        value = file_sizes.get(file_key)
        if value is None:
            continue  # absent from the file entirely - not this check's job to flag
        page_row = by_name.get(page_row_name)
        if page_row is None:
            found.append(Mismatch(file_key, repr(value), "<no row found>", page_name))
            continue
        needed = _numbers_in(value)
        if needed:
            have = _numbers_in_text(page_row.text)
            missing = needed - have
            if missing:
                found.append(Mismatch(file_key, f"numbers {sorted(needed)}", page_row.text, page_name))
        word = RADIUS_WORD_MAP.get(file_key)
        if word is not None and needed:
            required = next(iter(needed))
            tied = _word_tied_number(page_row.text, word)
            if tied is not None and tied != required:
                found.append(Mismatch(file_key, f"{required} (tied to {word!r})", f"{word!r} is next to {tied}", page_name))
    return tuple(found)


def ambiguous_matches(name: str, pages: dict[str, Page]) -> tuple[str, ...]:
    """Which pages (by name) draw a sentence row named `name`, when more than one does.

    Empty when `name` is drawn on at most one page - the ordinary case. More than one is
    itself a refusal, not a silent first-match-wins (today this never happens:
    Layout.dc.html is never parsed, so no name is ever drawn on two in-scope pages).
    """
    found = tuple(page_name for page_name, page in pages.items() if any(row.name == name for row in page.sentences))
    return found if len(found) > 1 else ()


def reverse_missing(
    file_colour_names: frozenset[str],
    file_type_and_size_names: frozenset[str],
    page: Page,
    page_name: str,
    allow: frozenset[str] = REVERSE_ALLOW_LIST,
) -> tuple[Missing, ...]:
    """Every colour/sentence row drawn on `page` with no matching file entry, `allow` aside.

    The caller passes `allow` explicitly (rather than this reading the module constant on
    its own) so a name matched through a map (`RADIUS_MAP`) and a name genuinely exempted
    (`REVERSE_ALLOW_LIST`) are two different things the caller unions, not one hidden list.
    """
    found = []
    for row in page.colours:
        if row.name not in file_colour_names and row.name not in allow:
            found.append(Missing(row.name, f"dark={row.dark} light={row.light}", page_name))
    for row in page.sentences:
        if row.name not in file_type_and_size_names and row.name not in allow:
            found.append(Missing(row.name, row.text, page_name))
    return tuple(found)


def ansi_reverse_missing(file_themes: tuple[dict, ...], page: Page, page_name: str) -> tuple[Missing, ...]:
    """Every ANSI card drawn on `page` whose theme has no matching named colour in the file."""
    found = []
    for theme in file_themes:
        file_names = {key.replace("_", " ") for key in theme["ansi"]}
        for row in page.ansi:
            if row.theme == theme["name"] and row.name not in file_names:
                found.append(Missing(f"{theme['name']}.{row.name}", row.hex, page_name))
    return tuple(found)


# ---------------------------------------------------------------------------
# The shell: reads the data file and the two pages, prints, exits
# ---------------------------------------------------------------------------


def _read_data_file(path: Path) -> dict[str, object]:
    """Read `crates/ui/design/foundations.json` as the raw document.

    `steg_tools.foundations.read()` (TASK-1.94, merged) reads `colours` and derives
    `opacity_names` only - everything else this module reads (`type`, `sizes`, `themes`,
    `motion`, `ramps`, `colour_aliases`, `tint`, `focus_ring`) is outside its scope, so
    this stays the one read of the whole document rather than reading the file twice
    down two different paths for the colours alone (see the module docstring).
    """
    with path.open() as handle:
        data = json.load(handle)
    assert isinstance(data, dict)
    return data


def main(argv: list[str] | None = None) -> int:
    """Check the real `crates/ui/design/foundations.json` against the real saved pages.

    `argv` defaults to the real command line; a test passes its own list, and
    `--data-file <path>` reads the data file from `<path>` instead of the repository's
    real one, so a negative control can mutate a scratch copy while this still reads the
    repository's real saved pages.
    """
    args = sys.argv[1:] if argv is None else argv
    data_override: str | None = None
    if len(args) == 2 and args[0] == "--data-file":
        data_override = args[1]
    elif args:
        print("usage: steg-design-check [--data-file <path>]", file=sys.stderr)
        return 2

    root = workspace.repo_root()
    if root is None:
        print("steg-design-check: not in a git repository", file=sys.stderr)
        return 2

    data_path = Path(data_override) if data_override else root / "crates/ui/design/foundations.json"
    data = _read_data_file(data_path)
    tokens_page = parse_page((root / "design/canvas/project/Tokens.dc.html").read_text())
    details_page = parse_page((root / "design/canvas/project/Details.dc.html").read_text())
    pages = {"Tokens.dc.html": tokens_page, "Details.dc.html": details_page}

    problems = shape_problems(pages)
    for problem in problems:
        print(f"steg-design-check: {problem}", file=sys.stderr)
    if problems:
        return 1

    unaccounted = unaccounted_names(data)
    for name in unaccounted:
        print(f"steg-design-check: {name} is neither checked nor declared unseen (TIER_C_SKIPS) - add it to one", file=sys.stderr)
    if unaccounted:
        return 1

    file_type = data.get("type", {})
    file_sizes = data.get("sizes", {})
    assert isinstance(file_type, dict)
    assert isinstance(file_sizes, dict)

    mismatches: list[Mismatch] = []
    mismatches.extend(colour_mismatches(tuple(data["colours"]), tokens_page, "Tokens.dc.html"))
    mismatches.extend(sentence_mismatches(file_type, file_sizes, tokens_page, "Tokens.dc.html"))
    mismatches.extend(mapped_sentence_mismatches(file_sizes, RADIUS_MAP, tokens_page, "Tokens.dc.html"))
    mismatches.extend(ansi_mismatches(tuple(data["themes"]), details_page, "Details.dc.html"))
    for name in TIER_B_NAMES:
        found_on = ambiguous_matches(name, pages)
        if len(found_on) > 1:
            mismatches.append(Mismatch(name, "<checked once>", f"drawn on more than one page: {', '.join(found_on)}", "/".join(found_on)))

    missing: list[Missing] = []
    missing.extend(
        reverse_missing(
            frozenset(entry["name"] for entry in data["colours"]),
            frozenset(file_type) | frozenset(file_sizes),
            tokens_page,
            "Tokens.dc.html",
            allow=REVERSE_ALLOW_LIST | frozenset(RADIUS_MAP.values()),
        )
    )
    for theme in data["themes"]:
        missing.extend(ansi_reverse_missing((theme,), details_page, "Details.dc.html"))

    for mismatch in mismatches:
        print(
            f"steg-design-check: {mismatch.name} does not match - "
            f"{data_path} has {mismatch.file_form}, {mismatch.page} has {mismatch.page_form}"
        )
    for gap in missing:
        print(f"steg-design-check: {gap.name} is drawn on {gap.page} ({gap.page_form}) with no entry in {data_path}")

    if mismatches or missing:
        return 1

    checked_colours = len([c for c in data["colours"] if c["name"] not in SKIPPED_COLOURS])
    checked_ansi = sum(len(theme["ansi"]) for theme in data["themes"])
    checked_sentences = len(TIER_B_NAMES) + len(RADIUS_MAP)
    tightened = sum(len(fields) for fields in FIELD_WORD_MAPS.values()) + len(RADIUS_WORD_MAP)
    print(f"steg-design-check: ok ({checked_colours} colours, {checked_sentences} type/size entries, {checked_ansi} ANSI colours checked)")
    print(
        f"steg-design-check: field-blind by default - a Tier B/mapped entry's numbers are only checked as a set "
        f"(two of its own fields swapped would still pass) unless FIELD_WORD_MAPS/RADIUS_WORD_MAP tightens that "
        f"field by tying it to the word that follows its number on the page; {tightened} fields are tightened "
        f"this way today, every other number in every Tier B/mapped entry is still only checked for presence"
    )
    for skip in TIER_C_SKIPS:
        print(f"steg-design-check: not checked - {skip.display()}: {skip.reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
