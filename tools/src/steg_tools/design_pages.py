"""Check `crates/ui/design/foundations.json` against the saved foundations pages' rendered markup.

TASK-1.96 (doc-17, R121, R122). A named value in the data file is compared against the same
name's row on `design/canvas/project/Tokens.dc.html` or `Details.dc.html`, by three tiers:

- Tier A (`colour_mismatches`, `ansi_mismatches`): name and value match exactly, both
  directions. The 17 of 18 colours with a Name/Dark/Light/Used for row on Tokens.dc.html
  (`colours[]` minus `track`, which has no named row anywhere in either page's rendered
  body - a declared exception, not a gap: see `TIER_C_SKIPS`), and all 32 ANSI colours
  (16 names x 2 themes) on Details.dc.html.
- Tier B (`sentence_mismatches`): the name is present and every number reachable in the
  file's entry is found somewhere in that same name's sentence cell - weaker than Tier A,
  since it cannot see a number land in the wrong field. `TIER_B_NAMES` is the exhaustive
  list of file keys this covers.
- Tier C (`TIER_C_SKIPS`): every other named value in the file is declared out of this
  tool's reach, each with its own reason - printed by `main()` itself on every passing run,
  so a clean exit is never read as "every value checked" (AC#3).

Only the rendered body is read - everything between `</helmet>` and the page's own
`<script type="text/x-dc">` block - never that script block itself: it holds the page's
own preview colour variables (`ink`, `off`, `quiet`, `faint`, ...), the page's vocabulary,
not the design's names, and parsing it would make values such as `off` (already decided
against in TASK-1.92) look like page-named values missing from the file.

The reverse direction (`reverse_missing`, `ansi_reverse_missing`) lists every name drawn
under a Name/Dark/Light/Used for, Name/Value, or ANSI swatch row and reports one with no
matching file entry - restricted to exactly those rows (never Layout.dc.html, never a
Rules bullet or a Stories caption), against `REVERSE_ALLOW_LIST` for a page name
deliberately absent from the file (today: `"radius"`, which merges four file keys into
one row - see `TIER_C_SKIPS`). This is `sizes.radius.round`'s exact scenario from
TASK-1.92's review (a value drawn with no file entry), now automatic.

Agreement between the file and a page proves nothing about whether a value is part of the
*confirmed* design (TASK-1.92's one real miss, `overlay.layout.max_height_percent_of_window`,
was a value the page and the then-draft file agreed on; doc-10 flagged it as not yet
confirmed - a judgement this tool cannot make). This is a consistency check between two
documents, not a substitute for reading doc-10's open-questions list.

TASK-1.94 is adding a shared `steg_tools.foundations.read()` for the data file; this task
imports it once it has merged. Until then, `_read_data_file` below is the few lines that
stand in for it (see TASK-1.96's plan, "Module shape").

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
    """A data-file value this tool declares out of its reach, and why (AC#3)."""

    name: str
    reason: str


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
    ExpectedShape("Tokens.dc.html", "colours", 17),
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
    Skipped(
        "colours.track",
        'no Name/Dark/Light/Used for row on either page - drawn only in each page\'s own '
        "script-block colour variables, never under its own name",
    ),
    Skipped("colour_aliases.blocked", "a reference to ramp.5 amber, never drawn under its own name"),
    Skipped("colour_aliases.done", "a reference to ramp.6 green, never drawn under its own name"),
    Skipped("colour_aliases.error", "a reference to ramp.3 red, never drawn under its own name"),
    Skipped("opacities.faint", "drawn only in each page's own script-block colour variables (0.3/0.45), never under its own name"),
    Skipped("ramps.ramp", "the gradient is drawn; its per-stop percentages are only in the page's own script block"),
    Skipped("ramps.ramp.extended", "the gradient is drawn; its per-stop percentages are only in the page's own script block"),
    Skipped("ramps.ramp.caption_opacity", "50%/40% is in Rules prose with no name column"),
    Skipped(
        "tint",
        'drawn as Layout.dc.html\'s Track card row named "fill", not under the name "tint" - '
        "and Layout.dc.html is out of scope entirely (its own text: \"values... are on the "
        'first Foundations board"); see sizes.row.gap below',
    ),
    Skipped("motion.status-fade", "200ms is in Rules prose with no name column"),
    Skipped("focus_ring.corner_radius_addition", "derived (12px corner minus 8px corner); 4 never appears as a number on the page"),
    Skipped("themes[].bg", "a trailing prose line per theme, not a named row"),
    Skipped("themes[].fg", "a trailing prose line per theme, not a named row"),
    Skipped("themes[].bg_inactive", "a trailing prose line per theme, not a named row"),
    Skipped("themes[].selection", "a trailing prose line per theme, not a named row"),
    Skipped("sizes.radius.surface", 'the page has one merged row "radius" for all three corner sizes plus "round"; no 1:1 name match'),
    Skipped("sizes.radius.track", 'the page has one merged row "radius" for all three corner sizes plus "round"; no 1:1 name match'),
    Skipped("sizes.radius.inner", 'the page has one merged row "radius" for all three corner sizes plus "round"; no 1:1 name match'),
    Skipped("sizes.radius.round", 'drawn as an unlabelled swatch ("fully round: chips of colour, bars"), no px number and no name at all'),
    Skipped("sizes.row.gap", "only on Layout.dc.html, out of scope entirely"),
    Skipped("sizes.stack.gap", "only on Layout.dc.html, out of scope entirely"),
    Skipped("sizes.track.layout", "only on Layout.dc.html, out of scope entirely"),
    Skipped("sizes.split.layout", "only on Layout.dc.html, out of scope entirely"),
    Skipped("sizes.overlay.layout", "only on Layout.dc.html, out of scope entirely"),
    Skipped("session", "a runtime sentinel (the active session's own colour); no fixed page form"),
)


def _skip_names(prefix: str) -> frozenset[str]:
    """Derive a bare-name skip set from `TIER_C_SKIPS`, so the skip list is the one source."""
    return frozenset(skip.name.removeprefix(prefix) for skip in TIER_C_SKIPS if skip.name.startswith(prefix))


SKIPPED_COLOURS = _skip_names("colours.")

# Tokens.dc.html's Sizes table: merges sizes.radius.surface/track/inner/round into one row
# named "radius" with no 1:1 file key - see TIER_C_SKIPS above.
REVERSE_ALLOW_LIST = frozenset({"radius"})

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


def sentence_mismatches(file_type: dict, file_sizes: dict, page: Page, page_name: str) -> tuple[Mismatch, ...]:
    """Tier B: every `TIER_B_NAMES` entry present in the file against its named page row.

    A name with no numbers in the file (`font.ui`, `font.terminal`) degrades to a
    name-presence check only - documented here, not hidden.
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
        if not needed:
            continue
        have = _numbers_in_text(page_row.text)
        missing = needed - have
        if missing:
            found.append(Mismatch(name, f"numbers {sorted(needed)}", page_row.text, page_name))
    return tuple(found)


def ambiguous_matches(name: str, pages: dict[str, Page]) -> tuple[str, ...]:
    """Which pages (by name) draw a sentence row named `name`, when more than one does.

    Empty when `name` is drawn on at most one page - the ordinary case. More than one is
    itself a refusal, not a silent first-match-wins (today this never happens:
    Layout.dc.html is never parsed, so no name is ever drawn on two in-scope pages).
    """
    found = tuple(page_name for page_name, page in pages.items() if any(row.name == name for row in page.sentences))
    return found if len(found) > 1 else ()


def reverse_missing(file_colour_names: frozenset[str], file_type_and_size_names: frozenset[str], page: Page, page_name: str) -> tuple[Missing, ...]:
    """Every colour/sentence row drawn on `page` with no matching file entry, `REVERSE_ALLOW_LIST` aside."""
    found = []
    for row in page.colours:
        if row.name not in file_colour_names and row.name not in REVERSE_ALLOW_LIST:
            found.append(Missing(row.name, f"dark={row.dark} light={row.light}", page_name))
    for row in page.sentences:
        if row.name not in file_type_and_size_names and row.name not in REVERSE_ALLOW_LIST:
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
    """Read `crates/ui/design/foundations.json`.

    TASK-1.94 is adding a shared `steg_tools.foundations.read()` for this file; once it
    has merged, this function is replaced by that import (TASK-1.96's plan, "Module
    shape"). Until then, this is the few lines the plan allows as a private fallback.
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

    file_type = data.get("type", {})
    file_sizes = data.get("sizes", {})
    assert isinstance(file_type, dict)
    assert isinstance(file_sizes, dict)

    mismatches: list[Mismatch] = []
    mismatches.extend(colour_mismatches(tuple(data["colours"]), tokens_page, "Tokens.dc.html"))
    mismatches.extend(sentence_mismatches(file_type, file_sizes, tokens_page, "Tokens.dc.html"))
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
    print(f"steg-design-check: ok ({checked_colours} colours, {len(TIER_B_NAMES)} type/size entries, {checked_ansi} ANSI colours checked)")
    for skip in TIER_C_SKIPS:
        print(f"steg-design-check: not checked - {skip.name}: {skip.reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
